# Librsvg releases with security fixes

Note that releases with an odd minor number (e.g. 2.47.x since
47 is odd) are considered development releases and should not be used
in production systems.

The following list is only for stable release streams, where the minor
number is even (e.g. 2.50.x).

### 2.50.4

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

### 2.48.10

CVE-2020-35905 - RUSTSEC-2020-0059 - data race in futures-util.

CVE-2020-35906 - RUSTSEC-2020-0060 - use-after-free in futures-task.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

### 2.48.0

CVE-2019-20446 - guard against exponential growth of CPU time
from malicious SVGs.

### 2.46.5

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

### 2.44.17

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

CVE-2019-15554 - RUSTSEC-2019-0012 - memory corruption in smallvec.

CVE-2019-15551 - RUSTSEC-2019-0009 - double-free and use-after-free in smallvec.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

### 2.44.16

CVE-2019-20446 - guard against exponential growth of CPU time
from malicious SVGs.

### 2.42.8

CVE-2019-20446 - guard against exponential growth of CPU time
from malicious SVGs.

### 2.42.9 

CVE-2018-20991 - RUSTSEC-2018-0003 - double-free in smallvec.

### 2.40.21

CVE-2019-20446 - guard against exponential growth of CPU time
from malicious SVGs.

### 2.40.18

CVE-2017-11464 - Fix division-by-zero in the Gaussian blur code.

### Earlier releases should be avoided and are not listed here.
