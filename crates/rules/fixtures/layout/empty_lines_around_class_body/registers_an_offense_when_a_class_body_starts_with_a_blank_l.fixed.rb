class SomeClass < Struct.new(
  :attr,
  keyword_init: true
)
  do_something
end
