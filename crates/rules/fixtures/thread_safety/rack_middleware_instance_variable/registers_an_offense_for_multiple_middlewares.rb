module MyMiddlewares
  class TestMiddleware
    def initialize(app)
      @app = app
      @foo = 1
      ^^^^^^^^ Avoid instance variables in Rack middleware.
    end

    def call(env)
      @app.call(env)
    end
  end

  class TestMiddleware2
    def initialize(app)
      @app = app
      @foo = 1
      ^^^^^^^^ Avoid instance variables in Rack middleware.
    end

    def call(env)
      @app.call(env)
    end
  end
end
