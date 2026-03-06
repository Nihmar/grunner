# Security Policy

## Supported Versions

This section informs users about which versions of grunner are currently being supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

**Note**: Version 1.0.0 is the only supported version. Security updates will be backported to this version.

## Reporting a Vulnerability

If you discover a vulnerability in grunner, please report it responsibly.

### How to Report

Please report vulnerabilities via:
- **GitHub Security Advisories**: Click "Security Advisory" on the repository page
- **Direct Message**: Reach out to the maintainer through GitHub

### What to Include

When reporting a vulnerability, please provide:

- A clear description of the vulnerability
- Steps to reproduce the issue
- Potential impact/exploit scenario
- Suggested fix (if applicable)
- Any relevant log output or error messages

### Response Timeline

| Action                               | Timeframe        |
| ------------------------------------ | ---------------- |
| Initial acknowledgment               | 48 hours         |
| Security review                     | 1 week           |
| Public advisory (if applicable)     | 2 weeks (max)    |
| Fix deployed                        | 2 weeks (max)    |

### Update Schedule

You can expect updates on the reported vulnerability:

- **Day 1-2**: Initial acknowledgment and triage
- **Day 3-7**: Security review and assessment
- **Day 8-14**: Fix development and testing

If the vulnerability is accepted:
- A fix will be developed and tested
- The fix will be released as a new version of 1.0.x
- A security advisory will be published with affected versions and mitigation guidance

If the vulnerability is declined:
- You will receive an explanation
- A public advisory may still be published for transparency

### Disclosure Policy

- All vulnerabilities will be handled with full transparency
- Security advisories will follow GitHub's best practices
- Affected users will be notified via GitHub security notifications
- A changelog entry will be added to the `CHANGELOG.md`

### Mitigation Recommendations

While waiting for a fix, users can:

1. Monitor the repository for security advisories
2. Check the GitHub Security tab for any active advisories
3. Review the security announcement for specific mitigation steps
4. Consider temporarily avoiding vulnerable features if advised

### Scope

This security policy covers:

- Remote code execution vulnerabilities
- Information disclosure vulnerabilities
- Authentication bypass vulnerabilities
- Privilege escalation vulnerabilities
- Denial of service vulnerabilities
- Any other security issues affecting grunner

Out of scope:
- Security issues already reported and in progress
- Issues that have no security impact
- Issues that are already mitigated by user configuration

### Contact

For questions about this security policy, please open an issue in the repository.
