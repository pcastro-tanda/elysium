class LoginController < ApplicationController
  before_action :require_login, except: 'health_check'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `health_check` is not explicitly defined on the class.

  def index
  end
end
