class FooController
  def action
    t 'foo.action.key'
      ^^^^^^^^^^^^^^^^ Use lazy lookup for the text used in controllers.
    translate 'foo.action.key'
              ^^^^^^^^^^^^^^^^ Use lazy lookup for the text used in controllers.
  end
end
