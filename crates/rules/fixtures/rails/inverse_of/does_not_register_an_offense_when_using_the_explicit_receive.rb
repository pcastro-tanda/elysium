class Person
  with_options inverse_of: :bar do |assoc|
    assoc.belongs_to :foo, foreign_key: 'foo_id'
  end
end
