class Person
  has_many :foo, conditions: -> { where(bar: true) }
  ^^^^^^^^ Specify an `:inverse_of` option.
end
