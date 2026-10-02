class Person
  with_options foreign_key: 'foo_id' do |assoc|
    assoc.belongs_to :foo, -> () { where(baz: true) }
          ^^^^^^^^^^ Specify an `:inverse_of` option.
  end
end
