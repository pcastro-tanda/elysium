module Bar
  class FooController
    def action
      t 'bar.foo.action.key'
        ^^^^^^^^^^^^^^^^^^^^ Use lazy lookup for the text used in controllers.
      t 'foo.action.key'
    end
  end
end
