#include "config.h"
#include "mainwindow.h"
#include "progressstatus.h"
#include "titlesmodel.h"
#include "tvrank.h"
#include "tvrankservice.h"
#include "tvrankserviceinit.h"
#include <cstdint>
#include <iostream>
#include <QtConcurrent>
#include <QtWidgets>

int main(int argc, char *argv[])
{
  QApplication a {argc, argv};
  a.setApplicationName("TVrank");
  a.setApplicationVersion(TVRANK_VERSION);

  TitlesModel moviesModel {};
  TitlesModel seriesModel {};

  TVrankService service {};
  TVrankServiceInit serviceInit {service};
  MainWindow w {moviesModel, seriesModel};
  w.show();

  QObject::connect(&serviceInit,
                   &TVrankServiceInit::contentLen,
                   &w,
                   &MainWindow::tvrankServiceContentLen);
  QObject::connect(&serviceInit,
                   &TVrankServiceInit::progress,
                   &w,
                   &MainWindow::tvrankServiceProgress);
  QObject::connect(&serviceInit, &TVrankServiceInit::failed, &w, &MainWindow::tvrankServiceFailed);
  QObject::connect(&serviceInit,
                   &TVrankServiceInit::success,
                   &w,
                   &MainWindow::tvrankServiceSucceeded);

  serviceInit.start();

  return a.exec();
}
