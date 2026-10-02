class Person
  has_many :foo, -> () { where(bar: true) }, inverse_of: nil
  ^^^^^^^^ You specified `inverse_of: nil`, you probably meant to use `inverse_of: false`.
end
