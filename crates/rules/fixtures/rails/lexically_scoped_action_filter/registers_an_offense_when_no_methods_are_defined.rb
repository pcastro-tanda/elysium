class LoginController < ApplicationController
  before_action :require_login, only: %i[index show]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `index`, `show` are not explicitly defined on the class.
end
