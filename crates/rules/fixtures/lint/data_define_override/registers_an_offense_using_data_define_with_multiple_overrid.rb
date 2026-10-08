Data.define(:members, :clone, :to_s)
                              ^^^^^ `:to_s` member overrides `Data#to_s` and it may be unexpected.
                      ^^^^^^ `:clone` member overrides `Data#clone` and it may be unexpected.
            ^^^^^^^^ `:members` member overrides `Data#members` and it may be unexpected.
