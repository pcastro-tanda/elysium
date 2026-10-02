class Person
  has_many :foo, dependent: :destroy, foreign_key: 'foo_id'
  ^^^^^^^^ Specify an `:inverse_of` option.
end
