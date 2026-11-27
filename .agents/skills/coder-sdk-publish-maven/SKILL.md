---
name: coder-sdk-publish-maven
description: |
  Build, validate, and publish the LMMS Java SDK. Public Maven Central
  publishing is tag-only and requires manual publication in Sonatype Central.
  Internal Reposilite publishing remains a separate local workflow.
---

# coder-sdk-publish-maven

Maven groupId: `io.github.luqizheng`. Java source packages remain `io.digua.*`.
The seven active modules are aggregated by `sdk/java/pom.xml`.

## Mandatory release rules

- Maven Central upload only on a Git tag matching `vMAJOR.MINOR.PATCH`.
- Pull requests and branch pushes never upload artifacts.
- No `workflow_dispatch` publishing entry point.
- Central plugin uses `autoPublish=false`; a human reviews and clicks Publish.
- Never create/push a release tag without explicit user authorization.
- Never run Central deploy during ordinary validation.
- Do not change Rust, npm, Docker, or existing internal release workflows.

## Validation

Use JDK 21 and Maven 3.9+:

```bash
mvn -B -f sdk/java/pom.xml -Pjava8 clean verify
mvn -B -f sdk/java/pom.xml -Pjava17 clean verify
mvn -B -f sdk/java/pom.xml -Pjava21 clean verify
```

Parent POM attaches Sources and Javadoc. GPG signing is opt-in via
`central-release`; Central upload is opt-in via `central-publishing`.

## GitHub Actions Secrets

- `MVN_USERNAME`: Sonatype Central User Token username (already configured).
- `MVN_PASSWORD`: Sonatype Central User Token password (already configured).
- `GPG_PRIVATE_KEY`: ASCII-armored secret key (not ready yet).
- `GPG_PASSPHRASE`: private-key passphrase (not ready yet).

The tag workflow fails before upload if any secret is missing. After a
successful upload, inspect https://central.sonatype.com/publishing/deployments
and manually click Publish.

## Internal Reposilite

The default `distributionManagement` and existing CLI remain for internal
publishing. Do not use Central User Token for Reposilite.
