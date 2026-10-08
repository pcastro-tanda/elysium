class Person
  with_options inverse_of: :bar do |_assoc|
    belongs_to :foo, -> () { where(baz: true) }
    ^^^^^^^^^^ Specify an `:inverse_of` option.
  end
end
