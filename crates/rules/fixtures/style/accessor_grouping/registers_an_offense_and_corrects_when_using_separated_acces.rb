class Foo
  attr_reader :bar1
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_reader :bar2
  ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  attr_accessor :quux
  attr_reader :bar3, :bar4
  ^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  other_macro :zoo
end
