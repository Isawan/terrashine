#!/bin/sh
set -e
source /venv/bin/activate
exec wsgi.py
