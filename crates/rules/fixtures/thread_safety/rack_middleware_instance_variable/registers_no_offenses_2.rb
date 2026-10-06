class TestMiddleware
  def initialize(app)
    @app = app
    instance_variable_set(1)
  end

  def call(env)
    @app.call(env)
    instance_variable_get({})
  end
end
