# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.2.3 - 2023-03-22
### Changed
- Improve Windows support.

## 0.2.0 - 2018-11-14
### Added
- Add support for `no_std`.
- Add Support for Windows.
- Add test for Windows and OS X.
- Add benchmark.
- Implement a fallback for old Linux systems.

### Changed
- Change the API in a backward-incompatible manner.

### Removed
- Remove the memory barrier normal path. Use `fence(Ordering::SeqCst)` instead.

## 0.1.0 - 2018-03-29
### Added
- First version of membarrier-rs.

[Unreleased]: https://github.com/jeehoonkang/membarrier-rs/compare/v0.1.0...HEAD
