module Foo
  class Regexp
  end

  puts $1
       ^^ Prefer `::Regexp.last_match(1)` over `$1`.
end
