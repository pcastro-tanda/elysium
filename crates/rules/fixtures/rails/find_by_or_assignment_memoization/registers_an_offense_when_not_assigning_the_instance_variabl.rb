class Foo
  def initialize
    @not_current_user = nil
  end

  def current_user
    @current_user ||= User.find_by(id: session[:user_id])
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid memoizing `find_by` results with `||=`.
  end
end
