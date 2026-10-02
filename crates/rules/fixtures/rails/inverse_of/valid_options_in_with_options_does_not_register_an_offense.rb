class Person
  with_options inverse_of: false do
    has_one :foo, -> () { where(bar: true) }
  end
end
