class Person
  belongs_to :foo, foreign_key: 'foo_id'
  ^^^^^^^^^^ Specify an `:inverse_of` option.
end
