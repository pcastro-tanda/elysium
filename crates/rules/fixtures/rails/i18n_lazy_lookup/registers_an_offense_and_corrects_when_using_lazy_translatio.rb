class FooController
  def action
    t '.key'
      ^^^^^^ Use explicit lookup for the text used in controllers.
    translate '.key'
              ^^^^^^ Use explicit lookup for the text used in controllers.
  end
end
