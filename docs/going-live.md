# Going live: what is set by hand

Rustergraphia ships the way cistergraphia does ([its pull request #69](https://github.com/chussenot/cistergraphia/pull/69)
is the design, read as a security reviewer and as an attacker would): a merge to `master`, once
CI is green, builds the container, pushes it to Artifact Registry and deploys it to Cloud Run;
with a domain, Cloudflare stands in front. Nothing is stored and nothing is rotated by hand: the
deploy proves who it is to Google with GitHub's OpenID token.

Terraform (`infra/`) makes what it can. This is the rest: every setting made once, by hand, on
Google Cloud, GitHub and Cloudflare, in the order it is needed. Done in this order, the first
deploy is the last step.

Three identities hold everything: your Google account (owner of the project), your GitHub
account (admin of the repository), your Cloudflare account (owner of the zone). Each gets
two-factor authentication with a passkey or a hardware key, not SMS, before anything below.

## What runs

| Piece | What it is | Where it is set |
|---|---|---|
| The container | `rustergraphia-web` on Google's distroless image (glibc, no shell), as `nonroot`, about 48 MB | `Dockerfile` |
| The service | Cloud Run, `europe-west1`: 1 vCPU, 256 MiB, 10 requests at once per instance, 0 to `max_instances` instances, CPU only during requests, probes on `/up` | `infra/gcp.tf` |
| The front door | A Cloudflare Worker on the domain: re-addresses each request to the service, adds the key the service demands, refuses odd methods and bodies over 3 MB, caches GETs as each page allows | `infra/worker.js`, `infra/cloudflare.tf` |
| The zone | The rate limit on the posts (10 in 10 s per address), HTTPS forced, TLS 1.2, the Free Managed Ruleset, the records saying the domain sends no mail | `infra/cloudflare.tf` |
| The bill | A budget with warnings, an alert on errors, an alert when every instance is busy | `infra/monitoring.tf` |
| The pipeline | CI (tests, interop with the Ruby original, the container built, smoke-tested and scanned, the Terraform validated and scanned), then Deploy | `.github/workflows/` |

Why these differ from cistergraphia's: a request is at most about 0.2 s of CPU here (a sealed
plate's scrypt) where Ruby's is up to 1.3 s behind a global lock, so one instance serves ten at
once instead of three, in half the memory; and the server starts in milliseconds, so scaling to
zero costs the first reader nothing noticeable.

## 0. Before anything: the default branch

**Settings, General, Default branch: `master`.** The repository was created with its first pull
request's branch as default. Two things depend on it being `master`: a workflow triggered by
another's completion (Deploy, after CI) runs from the default branch's copy of itself, and the
federation admits only a token whose `workflow_ref` is `deploy.yml@refs/heads/master`. With any
other default branch, the deploy never starts, or Google refuses it.

## Alongside cistergraphia, or alone

Both sites can share one Google project and one Cloudflare zone, or each have its own. A project
of its own is the default and the simplest to reason about. If they share:

- **The project.** Every name is drawn from `service_name` (`rustergraphia`): the service, the
  image repository, both service accounts, the secret, and the federation's pool
  (`rustergraphia-github`, where cistergraphia's is `github`). Each site's deployer can deploy
  its own service only. Use a different state prefix (it is: `rustergraphia`, in
  `infra/versions.tf`) in the same bucket. The budget covers the whole project: set it once, on
  one of the two configurations (leave `billing_account_id` empty on the other).
- **The zone.** Give this site a host of cistergraphia's zone (`rust.cistergraphia.example`,
  say) and set `zone_settings = false`: the rate limit, the managed rules, HTTPS forced and TLS
  1.2 are zone-wide, and cistergraphia's configuration already owns them (its rate limit matches
  `/generate`, `/decode` and `/ask` on every host of the zone). The free plan allows one
  rate-limiting rule per zone, and two configurations writing the same zone settings would undo
  each other at every apply. The Worker, its custom domain and the mail records are per host,
  and stay this configuration's.

## 1. Google Cloud

**1.1 A project**, on a billing account (or cistergraphia's, above):

```
gcloud projects create PROJECT_ID
gcloud billing projects link PROJECT_ID --billing-account XXXXXX-XXXXXX-XXXXXX
gcloud config set project PROJECT_ID
gcloud auth application-default login     # what Terraform signs in with
```

**1.2 The state bucket**, the one thing Terraform cannot make for itself. The state holds the
front door's key, so the bucket is private, versioned, and can never be made public:

```
gcloud storage buckets create gs://STATE_BUCKET --project PROJECT_ID --location europe-west1 \
  --uniform-bucket-level-access --public-access-prevention
gcloud storage buckets update gs://STATE_BUCKET --versioning
```

Who can read it is who has `roles/viewer` or more on the project. Keep the project's members to
yourself: `gcloud projects get-iam-policy PROJECT_ID`.

**1.3 For the budget** (`billing_account_id`): the account applying needs *Billing Account Costs
Manager* on the billing account. Without it, leave `billing_account_id` empty and set a budget in
the console (Billing, Budgets & alerts).

**1.4 Apply**, as the project owner:

```
cd infra
cp terraform.tfvars.example terraform.tfvars   # project_id; alert_email; the domain later (step 3)
terraform init -backend-config="bucket=STATE_BUCKET"
terraform apply
terraform output github_variables              # for step 2.2
```

The service starts on Google's hello image; the first deploy replaces it.

**1.5 Where the security mail goes** (Google writes about abuse, outages and security to the
project's Essential Contacts; without one, to nobody):

```
gcloud essential-contacts create --email=you@example.com --notification-categories=SECURITY,BILLING,TECHNICAL,SUSPENSION
```

**1.6 In an organization** (otherwise skip): `iam.disableServiceAccountKeyCreation`,
`iam.automaticIamGrantsForDefaultServiceAccounts`, `storage.publicAccessPrevention` cost nothing
and close doors the project never uses. Do **not** enforce `iam.allowedPolicyMemberDomains`: the
service is open to `allUsers`.

## 2. GitHub (Settings of the repository)

**2.1 Actions, General.**

| Setting | Value | Why |
|---|---|---|
| Actions permissions | *Allow OWNER, and select non-OWNER, actions*: *Allow actions created by GitHub*, plus `Swatinem/rust-cache@*`, `ruby/setup-ruby@*`, `docker/*`, `google-github-actions/*`, `hashicorp/setup-terraform@*` | Only the actions the workflows name can run; the Rust toolchain comes from the runner's own `rustup` |
| Require actions to be pinned to a full-length commit SHA | on | The workflows pin every action to a commit; this refuses one that forgets |
| Fork pull request workflows | *Require approval for all external contributors* | A fork's pull request builds the Dockerfile and runs the tests: nobody's code runs on your runners until you have read it |
| Workflow permissions | *Read repository contents and packages permissions*; *Allow GitHub Actions to create and approve pull requests* off | The workflows ask for `contents: read` and, the deploy alone, `id-token: write` |

**2.2 Environments: `production`.**

- *Deployment branches and tags*: *Selected branches and tags*, `master`.
- *Environment variables*: the five that `terraform output github_variables` prints
  (`GCP_PROJECT_ID`, `GCP_REGION`, `GCP_SERVICE_ACCOUNT`, `GCP_WORKLOAD_IDENTITY_PROVIDER`,
  `SITE_URL`). Variables, not secrets: none of them is secret, and the deploy skips until they
  exist.
- *Required reviewers*: optional. With yourself as reviewer, every deploy waits for a click.

**2.3 Rules, Rulesets: a branch ruleset on `master`**, bypass list empty:

| Rule | Why |
|---|---|
| Restrict deletions, block force pushes | The history CI tested is the history that deployed |
| Require a pull request before merging | Nothing reaches master unread, Dependabot's updates included |
| Require status checks to pass: `test`, `interop`, `container`, `infra` | The deploy already waits for CI; this stops a red master from existing at all |

**2.4 Code security** (free on a public repository): Dependabot alerts and security updates on;
secret scanning and push protection on; code scanning, *default setup* (CodeQL reads the Rust and
the JavaScript); private vulnerability reporting on.

## 3. Cloudflare (when there is a domain)

**3.1 The zone**, with DNSSEC on and the registrar's transfer lock and two-factor on. A domain
is the site: with the name, an attacker serves a copy of Inscribe and collects passphrases.

**3.2 The ids.** Zone *Overview*, *API*: *Zone ID* and *Account ID*, into `terraform.tfvars`
as `cloudflare_zone_id` and `cloudflare_account_id`, with `domain` (and `zone_settings = false`
on a zone cistergraphia already sets).

**3.3 The hostname is free.** The front door is a Workers custom domain, which makes the DNS
record itself: delete any A, AAAA or CNAME record on `domain` first.

**3.4 An API token** (*My Profile, API Tokens, Custom token*), never the Global API Key:

| Scope | Permission |
|---|---|
| Account, Workers Scripts | Edit |
| Zone, Workers Routes | Edit |
| Zone, Zone WAF | Edit (only with `zone_settings`) |
| Zone, Zone Settings | Edit (only with `zone_settings`) |
| Zone, DNS | Edit (the mail records, step 3.6) |
| Zone, Zone | Read |

The one account, the one zone, your address, expiring today. Give it to the shell for the apply
only:

```
read -rs CLOUDFLARE_API_TOKEN && export CLOUDFLARE_API_TOKEN   # not on the command line: the history keeps that
terraform apply
unset CLOUDFLARE_API_TOKEN
```

**3.5 The managed rules** (with `zone_settings`). If *Security, WAF, Managed rules* already shows
the Free Managed Ruleset deployed, import it first, or set `managed_waf = false`:

```
curl -s -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
  "https://api.cloudflare.com/client/v4/zones/ZONE_ID/rulesets" \
  | jq -r '.result[] | select(.phase == "http_request_firewall_managed") | .id'
terraform import 'cloudflare_ruleset.managed_waf[0]' zone/ZONE_ID/RULESET_ID
```

**3.6 Mail.** By default the domain says it sends no mail (null MX, SPF `-all`, DMARC
`p=reject`), so a message forged in the site's name is refused by every receiver that checks. If
you send mail from it, set `domain_sends_mail = true`.

**3.7 Bot Fight Mode** stays off: it challenges `curl`, which the deploy's smoke test is.

## 4. The first deploy, and what to check

1. *Actions, Deploy, Run workflow* on `master`, or merge to master. The job builds, pushes,
   deploys and smoke-tests through the front door.
2. From anywhere:

   ```
   deploy/smoke.sh https://DOMAIN                       # every page answers, HSTS is on
   curl -sI https://rustergraphia-xxxx.a.run.app/ | head -1   # 403: the back door is shut
   curl -sI https://rustergraphia-xxxx.a.run.app/up | head -1 # 200: the probes' door is open
   curl -sI -X PUT https://DOMAIN/ | head -1            # 405: refused at the edge
   for i in $(seq 12); do curl -so /dev/null -w '%{http_code} ' -d message=x https://DOMAIN/generate; done
                                                        # the last ones 429: the rate limit
   ```
3. The logs, if something is wrong: `gcloud run services logs read rustergraphia --region
   europe-west1 --limit 50`.

## 5. What holds a secret, and where

| Secret | Where it lives | Who can read it | Rotation |
|---|---|---|---|
| The front door's key | Secret Manager; the Terraform state; the Worker's secret binding | The runtime account (its one role); readers of the state bucket; the Cloudflare account | `terraform taint 'random_password.front_door_key[0]' && terraform apply`, then one deploy |
| Your Cloudflare API token | Your shell, during an apply | You | Expires the day it is made |
| Your Google credentials | `gcloud` on your machine | You | `gcloud auth revoke` |
| GitHub | nothing: the deploy proves who it is with a token GitHub mints per run | | |

## 6. Routine

- **Dependabot's pull requests**, weekly: the crates (`Cargo.lock` records each one's checksum,
  and the image is built `--locked`), the base images (tag and digest), the actions (commit and
  version), the providers. CI proves each.
- **A vulnerability** in a crate fails CI's `Cargo.lock` scan; in the base image, its image scan.
  The base image is Debian 13 distroless: the Debian 12 one carried an OpenSSL the binary never
  calls but the scan rightly counts.
- **The kill switch**: `gcloud run services update rustergraphia --region europe-west1 --ingress
  internal` closes the service in seconds.
- **Once a quarter**: `gcloud projects get-iam-policy PROJECT_ID`, the repository's collaborators
  and applications, the Cloudflare account's members and tokens.

## Checked before this was merged

- The image built from this Dockerfile (in a sandbox that needed its proxy's certificate for
  crates.io: the build stage given that, and nothing else changed): 48 MB, user `nonroot`.
- `deploy/smoke.sh` against it: every check. SIGTERM: stopped in 0.2 s, the requests in flight
  finished. With `FRONT_DOOR_KEY`: 403 without the key or with a wrong one, `/up` open, 200 with
  it; HSTS over HTTPS; a 3 MB post refused with 413 in 1.5 ms.
- Trivy: the image, no fixable HIGH or CRITICAL; `Cargo.lock`, none; `infra/`, no
  misconfiguration.
- `terraform fmt -check`, `init -backend=false`, `validate`, against google 6.50.0, cloudflare
  5.28.1, random 3.9.1 (the lock file is cistergraphia's, the same versions).
- Not done from here: `terraform apply` and the first deploy, which need your Google and
  Cloudflare accounts.
