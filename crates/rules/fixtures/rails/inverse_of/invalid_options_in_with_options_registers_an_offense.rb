class Person
  with_options foreign_key: 'foo_id' do
    has_one :foo
    ^^^^^^^ Specify an `:inverse_of` option.
  end
end
