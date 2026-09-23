module Foo::Bar
       ^^^^^^^^ Use compact module/class definition instead of nested style.
  module Baz
  end
end

module Foo
       ^^^ Use compact module/class definition instead of nested style.
  module Bar::Baz
  end
end
