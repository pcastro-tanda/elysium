class LoginController < ApplicationController
  before_action :require_login, except: 'health_check'

  def health_check
  end
end
