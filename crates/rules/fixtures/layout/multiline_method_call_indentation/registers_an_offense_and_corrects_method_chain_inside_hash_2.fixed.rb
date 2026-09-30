def payload
  {
    type: 'action',
    params: {
      page: get_page_name,
      email: @action.member.email,
      mailing_id: @mailing_id
    }.reverse_merge(@action.form_data)
       .merge(UserLanguageISO.for(page.language))
       .tap do |params|
         params[:country] = country(member.country) if member.country.present?
         params[:action_bucket] = data[:bucket] if data.key? :bucket
       end
  }.deep_symbolize_keys
end
