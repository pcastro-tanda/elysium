class FooController
  def action
    I18n.t 'foo.action.key'
    I18n.translate 'foo.action.key'
  end
end
