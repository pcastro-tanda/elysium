class Person
  has_many :foo, -> { group 'x' }, dependent: :destroy
  ^^^^^^^^ Specify an `:inverse_of` option.
end
