class Person
  def define_association(**options)
    has_many(:foo, -> { group 'x' }, dependent: :destroy, **options)
  end
end
