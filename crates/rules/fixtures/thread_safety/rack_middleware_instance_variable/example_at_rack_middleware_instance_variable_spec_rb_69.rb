class SomeClass
  def initialize(app)
    @app = app
    @user = User.new
  end

  def call(env, user)
    @x = TOPLEVEL_BINDING
  end
end
