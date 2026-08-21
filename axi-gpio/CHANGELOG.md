Change Log
=======

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

# [unreleased]

Initial implementation of the driver: `Input`/`Output` pin access via `ChannelId`/`Pin<N>`
ownership tokens, single- and dual-channel `AxiGpio` constructors, and an interrupt-driven
`asynch` module for awaiting per-channel events.
