Person = Struct.new(:first_name, :last_name) do
  def full_name
    "#{first_name} #{last_name}"
  end

  def foo
    a = 1
    a = 2
    a = 3
    a = 4
    a = 5
    a = 6
  end
end
