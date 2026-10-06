class HomeController < ApplicationController
  def create
    if foo.update(params)
      flash[:success] = 'msg'

      if redirect_to_index?
        redirect_to index
      else
        redirect_to path(foo)
      end
    else
      flash.now[:alert] = 'msg'
      render :edit, status: :unprocessable_entity
    end
  end
end
