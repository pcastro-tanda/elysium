class User
  validates
  after_commit :foo
  def foo; true; end
end
