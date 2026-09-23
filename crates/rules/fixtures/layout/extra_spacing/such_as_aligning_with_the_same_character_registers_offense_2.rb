y, m = (year * 12 + (mon - 1) + n).divmod(12)
m,   = (m + 1)                    .divmod(1)
              ^^^^^^^^^^^^^^^^^^^ Unnecessary spacing detected.
  ^^ Unnecessary spacing detected.
