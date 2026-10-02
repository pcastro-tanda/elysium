class Person
  has_many :pictures, as: :imageable
  ^^^^^^^^ Specify an `:inverse_of` option.
end
