SomeClass.module_eval do
  attr :name
  ^^^^ Do not use `attr`. Use `attr_reader` instead.
end
