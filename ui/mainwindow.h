#pragma once

#include "progressstatus.h"
#include "titlesmodel.h"
#include "tvrankserviceinit.h"
#include <QMainWindow>

QT_BEGIN_NAMESPACE
namespace Ui
{
  class MainWindow;
}
QT_END_NAMESPACE

class MainWindow: public QMainWindow
{
  Q_OBJECT

 public:
  MainWindow(TitlesModel& moviesModel, TitlesModel& seriesModel, QWidget *parent = nullptr);
  ~MainWindow();

  void tvrankServiceFailed(const enum TVrankServiceError err);
  void tvrankServiceSucceeded(const TVrankService& service);
  void tvrankServiceContentLen(const uint64_t len);
  void tvrankServiceProgress(const uint64_t delta);

 private:
  Ui::MainWindow *ui;
  ProgressStatus *progressStatusUi;

  TitlesModel& moviesModel;
  TitlesModel& seriesModel;

  void doSearch();
  void showAbout();
  void searchEditTextChanged();
};
