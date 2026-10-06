module Bar
  class FooController
    def action
      t '.key'
        ^^^^^^ Use explicit lookup for the text used in controllers.
      t 'foo.action.key'
    end
  end
end
