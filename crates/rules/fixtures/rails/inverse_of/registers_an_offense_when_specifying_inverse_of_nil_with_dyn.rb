class Person
  def define_association(**options)
    has_many :foo, -> () { where(bar: true) }, inverse_of: nil, **options
    ^^^^^^^^ You specified `inverse_of: nil`, you probably meant to use `inverse_of: false`.
  end
end
