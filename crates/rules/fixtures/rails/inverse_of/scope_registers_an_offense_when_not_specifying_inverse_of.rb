class Person
  has_one :foo, -> () { where(bar: true) }
  ^^^^^^^ Specify an `:inverse_of` option.
end
