class TestMiddleware
  def initialize(app)
    @app = app
    instance_variable_set(:@options, {})
  end

  def call(env)
    @app.call(env)
  end
end
