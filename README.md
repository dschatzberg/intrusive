Intrusive Data Structures in Rust
=======================

[![Build Status](https://travis-ci.org/dschatzberg/intrusive.svg?branch=master)](https://travis-ci.org/dschatzberg/intrusive)

This library aims to provide safe, useful intrusive data structures in the Rust
programming language. Intrusive data structures are data structures which do not
explicitly allocate memory. Instead they depend on the elements to contain the
necessary references to be inserted into the container. This is useful in cases
where memory allocation is not possible or needs to be tightly controlled.

[Documentation](https://dschatzberg.github.io/intrusive/intrusive_containers/index.html)

License
-------
Lesser GPL v3
