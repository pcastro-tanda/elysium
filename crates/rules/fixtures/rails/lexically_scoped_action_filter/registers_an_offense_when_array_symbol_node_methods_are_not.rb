class LoginController < ApplicationController
  before_action :require_login, only: %i[index settings logout]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `settings`, `logout` are not explicitly defined on the class.

  def index
  end
end
