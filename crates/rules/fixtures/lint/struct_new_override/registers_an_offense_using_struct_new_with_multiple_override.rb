Struct.new(:members, :clone, :zip)
                             ^^^^ `:zip` member overrides `Struct#zip` and it may be unexpected.
                     ^^^^^^ `:clone` member overrides `Struct#clone` and it may be unexpected.
           ^^^^^^^^ `:members` member overrides `Struct#members` and it may be unexpected.
