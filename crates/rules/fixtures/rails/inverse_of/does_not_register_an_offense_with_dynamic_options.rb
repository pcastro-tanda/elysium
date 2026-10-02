class Person
  def define_association(**options)
    has_many :foo, conditions: -> { where(bar: true) }, **options
  end
end
