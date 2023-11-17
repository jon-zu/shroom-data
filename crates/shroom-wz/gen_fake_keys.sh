#!/bin/sh

openssl rand -out "keys/gms_iv.bin" 16
openssl rand -out "keys/sea_iv.bin" 16
openssl rand -out "keys/default_iv.bin" 16


openssl rand -out "keys/aes.bin" 32
openssl rand -out "keys/wz_magic.bin" 4