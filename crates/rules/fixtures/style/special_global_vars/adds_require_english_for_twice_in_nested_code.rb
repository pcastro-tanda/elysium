# frozen_string_literal: true

puts $*[0]
     ^^ Prefer `$ARGV` from the stdlib 'English' module (don't forget to require it) or `ARGV` over `$*`.
puts $*[1]
     ^^ Prefer `$ARGV` from the stdlib 'English' module (don't forget to require it) or `ARGV` over `$*`.
