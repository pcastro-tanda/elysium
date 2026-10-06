class SomeClass
  def initialize(user, context)
    @user = user
    @context = context
  end

  def call
    [@user, @context]
  end
end
