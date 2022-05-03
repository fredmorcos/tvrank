#include "tvrankserviceinit.h"
#include "tvrankservice.h"

TVrankServiceInit::TVrankServiceInit(TVrankService& service, QObject *parent):
 QThread {parent},
 service(service)
{}

void TVrankServiceInit::run()
{
  TVrankServiceError serviceError;
  service.service = tvrank_service_new("/home/fred/.cache/tvrank",
                                       false,
                                       &TVrankServiceInit::serviceProgressUpdate,
                                       this,
                                       &serviceError);

  if (service.service == nullptr) {
    emit failed(serviceError);
  } else {
    emit success(service);
  }
}

void TVrankServiceInit::serviceProgressUpdate(void *data,
                                              const uint64_t *contentLen,
                                              uint64_t delta)
{
  auto service = static_cast<TVrankServiceInit *>(data);

  if (contentLen != nullptr) {
    emit service->contentLen(*contentLen);
  }

  emit service->progress(delta);
}
