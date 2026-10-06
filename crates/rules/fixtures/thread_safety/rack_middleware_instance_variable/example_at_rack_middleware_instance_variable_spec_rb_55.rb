class SomeClass
  def initialize(app)
    @user = User.new
  end

  def call(env)
    @x = TOPLEVEL_BINDING
  end
end
