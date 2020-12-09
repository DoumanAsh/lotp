# lotp (loli otp)

![Rust](https://github.com/DoumanAsh/lotp/workflows/Rust/badge.svg?branch=master)

Simple & small CLI tool to generate OTP (one time password).

## Security

When you open application first time it asks for passphrase in order to encrypt stored secrets.
It requires that both username (taken from system) and password matches to configuration, otherwise user is not allowed in.

In case you want to clear existing configuration, look for `.lotp.json` in folder with binary.
