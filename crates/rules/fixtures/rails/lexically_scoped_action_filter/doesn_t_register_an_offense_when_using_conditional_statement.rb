class Test < ActionController
  before_action(:authenticate, only: %i[update cancel]) unless foo

  def update; end

  def cancel; end
end
